use std::io;
use std::io::Read;
use unicode_segmentation::UnicodeSegmentation;

// TODO: There are non-printing code points such as ZWS - how are these handled?
/// Represents one "element" in a stream that is destined to end at a console
/// When manipulating such a stream, we generally want to iterate over these elements.
#[derive(Debug, Clone, Copy)]
pub enum ConsoleElem<'a> {
    Newline,
    CarriageReturn,
    Tab,
    OtherNonPrinting(char),
    Ansi(&'a str),
    Grapheme(&'a str),
    NonUTF8Data(u8),
}



/// Used internally by [IterElements], to track the amount of the slice that has been verified as
/// a str, or confirmed to be invalid
#[derive(Debug)]
enum KnownSegment<'a> {
    None,
    RawBytes(&'a [u8]),
    ValidUtf8(&'a str),
}


/// Used by [IterElements] to signal that the end of the slice is reached, and no new [ConsoleElement]s
/// can be emitted. [IterElements::slop_bytes] should be used to determine how many bytes couldn't
/// be consumed
struct NeedMoreData;


/// Used to iterate over the [ConsoleElement]s in a slice.
/// true_end indicates that the stream ends immediately after the last byte in this buffer.
/// If this is not set, then care is taken to ensure that elements that may be cut off at the edge of
/// the slice are not misinterpreted.
struct IterElements<'a> {
    remaining: &'a [u8],
    known_segment: KnownSegment<'a>,
    true_end: bool,
}


impl<'a> IterElements<'a> {
    fn new(bytes: &'a [u8], true_end: bool) -> Self {
        Self {
            remaining: bytes,
            known_segment: KnownSegment::None,
            true_end,
        }
    }

    fn consume(&mut self, amount: usize) {
        self.remaining = &self.remaining[amount..];
    }

    /// Loads the next segment from the slice into self.known_segment.
    fn try_fetch_next_known(&mut self) -> Result<(), NeedMoreData> {
        match std::str::from_utf8(self.remaining) {
            Ok(txt) => {
                self.known_segment = KnownSegment::ValidUtf8(txt);
                self.consume(self.remaining.len());
                Ok(())
            }

            Err(e) => {
                if e.valid_up_to() > 0 {
                    self.known_segment = KnownSegment::ValidUtf8(std::str::from_utf8(&self.remaining[..e.valid_up_to()]).unwrap());
                    self.consume(e.valid_up_to());
                    Ok(())

                } else if let Some(len) = e.error_len() {
                    self.known_segment = KnownSegment::RawBytes(&self.remaining[..len]);
                    self.consume(len);
                    Ok(())

                } else {
                    Err(NeedMoreData)
                }
            }
        }
    }

    fn consume_from_utf8(&mut self) -> Result<ConsoleElem, NeedMoreData> {
        let KnownSegment::ValidUtf8(mut remaining) =  self.known_segment
            else {panic!()};

        let r = if remaining.starts_with('\n') {
            remaining = &remaining[1..];
            Ok(ConsoleElem::Newline)

        } else if remaining.starts_with('\r') {
            remaining = &remaining[1..];
            Ok(ConsoleElem::CarriageReturn)

        } else if remaining.starts_with('\t') {
            remaining = &remaining[1..];
            Ok(ConsoleElem::Tab)

        } else if remaining.starts_with('\u{001B}') {
            let base = remaining;
            let mut length = 0;

            let mut valid_end_found = false;

            while let Some(next) = remaining.chars().next() {
                length += 1;
                remaining = &remaining[1..];
                if next > '\u{0040}' && (next != '[' || length > 2) {
                    valid_end_found = true;
                    break;
                }
            }

            // Cancels the consumption
            if !valid_end_found && !self.true_end {
                return Err(NeedMoreData);
            }

            Ok(ConsoleElem::Ansi(&base[0..length]))

        } else {
            let first_char = remaining.chars().next().ok_or(NeedMoreData)?;
            if first_char.is_ascii_control() {
                remaining = &remaining[1..];
                Ok(ConsoleElem::OtherNonPrinting(first_char))

            } else {
                let mut graphemes = remaining.grapheme_indices(true);
                let (_, grapheme) = graphemes.next()
                    .ok_or(NeedMoreData)?;

                let next = graphemes.next();

                let i = match next {
                    Some((j, _)) => j,
                    _ => {
                        if !self.true_end {
                            return Err(NeedMoreData);
                        }
                        remaining.len()
                    }
                };

                if next.is_none() && !self.true_end {
                    return Err(NeedMoreData);
                }

                remaining = &remaining[i..];
                Ok(ConsoleElem::Grapheme(grapheme))
            }
        };

        self.known_segment = match remaining {
            "" => KnownSegment::None,
            x => KnownSegment::ValidUtf8(x),
        };

        r

    }

    /// Produce an element by consuming raw bytes from the known_segment
    /// requires that known_segment is [KnownSegment::RawBytes]
    fn consume_from_raw(&mut self) -> Result<ConsoleElem, NeedMoreData> {
        let KnownSegment::RawBytes(mut remaining) = self.known_segment
            else {panic!("consume_from_raw called when known_segment was not RawBytes")};


        let byte = remaining[0];
        remaining = &remaining[1..];


        self.known_segment = match remaining {
            [] => KnownSegment::None,
            x => KnownSegment::RawBytes(x),
        };

        Ok(ConsoleElem::NonUTF8Data(byte))
    }


    /// Attempts to return the next [ConsoleElement] from the slice
    fn try_get_next_element(&mut self) -> Result<ConsoleElem, NeedMoreData> {
        if matches!(&self.known_segment, KnownSegment::None) {
            self.try_fetch_next_known()?;
        }

        match &self.known_segment {
            KnownSegment::None => unreachable!("try_fetch_next_known should set known_segment or else return Err(_)"),
            KnownSegment::RawBytes(_) => self.consume_from_raw(),
            KnownSegment::ValidUtf8(_) => self.consume_from_utf8()
        }
    }

    /// Returns the number of bytes that have not yet been consumed.
    /// Once we are unable to say for sure that the remaining bytes make up a whole element, we can
    /// continue to process them by adding these bytes to the front of a new buffer, then reading in
    /// the next bytes from the stream
    fn slop_bytes(&self) -> usize {
        // We have moved some of remaining to known_segment,
        // reconstruct what remaining would have been if we hadn't
        self.remaining.len() + match self.known_segment {
            KnownSegment::None => 0,
            KnownSegment::RawBytes(x) => x.len(),
            KnownSegment::ValidUtf8(x) => x.as_bytes().len(),
        }
    }
}


pub fn for_each_console_element<R, F>(mut i: R, mut f: F) -> io::Result<()>
    where R: Read,
          F: FnMut(ConsoleElem<'_>) -> io::Result<()> {
    let mut buffer = vec![0; 256];

    let mut already_hit_end;

    let amount = i.read(&mut buffer)?;
    already_hit_end = amount == 0;
    let mut iter = IterElements::new(&buffer[..amount], already_hit_end);

    let mut last_end = amount;

    loop {
        match iter.try_get_next_element() {
            Ok(elem) => f(elem)?,
            Err(_) => {
                if already_hit_end {
                    return Ok(());
                }
                let slop = iter.slop_bytes();

                buffer.copy_within(last_end-slop.., 0);

                let amount = i.read(&mut buffer[slop..])?;
                already_hit_end = amount == 0;
                iter = IterElements::new(&buffer[..(slop+amount)], already_hit_end);

                last_end = slop + amount;
            }
        }
    }
}