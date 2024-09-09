use std::io;
use std::io::{copy, Read, Write};
use image::{GenericImageView, Pixel, Rgb};

use crate::console::{parse_ansi_type, AnsiCodeType, Color, ConsoleElem, for_each_console_element};


/// Generic trait for anything which can "colorize" a stream. What exactly this means depends on the
/// implementation.
pub trait StreamColorizer {
    fn copy_colorized<I, O>(&mut self, input: I, output: O, config: &ColorizerConfig) -> io::Result<()>
        where I: Read, O: Write;
}


/// Configuration for a [StreamColorizer]
#[derive(Debug, Clone)]
pub struct ColorizerConfig {
    pub supports_rgb24: bool,
    pub wraps_after: Option<usize>,
    pub tab_size: usize,
    pub flush_on_newline: bool,
}


impl Default for ColorizerConfig {
    fn default() -> Self {
        Self {
            supports_rgb24: true,
            wraps_after: None,
            tab_size: 8,
            flush_on_newline: true,
        }
    }
}


/// A trait which represents objects which can colorize a stream based on the (expected) location of
/// each grapheme in the terminal.
/// Implementing this trait automatically provides an implementation of [StreamColorizer]
pub trait PositionalRecolorizer {
    fn get_color(&mut self, position: (usize, usize)) -> Color;
}


impl<T> StreamColorizer for T where T: PositionalRecolorizer {
    fn copy_colorized<I, O>(&mut self, input: I, mut output: O, config: &ColorizerConfig) -> io::Result<()> where I: Read, O: Write {
        let wrap_column = config.wraps_after.unwrap_or(usize::MAX);

        // Start at the top-left, and initialise the color for this position
        let mut position = (0, 0);
        let mut color = self.get_color(position);
        if config.supports_rgb24 {
            color.write_as_24bit_ansi(&mut output)?;
        } else {
            color.write_as_paletted_ansi(&mut output)?;
        }

        for_each_console_element(input, move |elem| {
            match elem {
                // Unix-style handling of carriage return - moves cursor to the beginning of the line
                ConsoleElem::CarriageReturn => {
                    position.0 = 0;
                    write!(output, "\r")?;
                },

                // Unix-style newline handling - move cursor to the beginning of the next line
                ConsoleElem::Newline => {
                    position.1 += 1;
                    position.0 = 0;
                    writeln!(output)?;
                    if config.flush_on_newline {
                        output.flush()?;
                    }
                },

                // Tab snaps the cursor to the next multiple of tab_size
                ConsoleElem::Tab => {
                    position.0 = ((position.0 / config.tab_size)+1) * config.tab_size;
                    if position.0 >= wrap_column {
                        position.0 = wrap_column - 1;
                    }
                    write!(output, "\t")?;
                }

                // We have to assume that each grapheme take up exactly one cell -
                // really it's up to the terminal how it displays each grapheme
                ConsoleElem::Grapheme(grapheme) => {
                    let new_color = self.get_color(position);
                    // TODO: More permissive equality when using paletted ansi
                    if new_color != color {
                        color = new_color;
                        if config.supports_rgb24 {
                            color.write_as_24bit_ansi(&mut output)?;
                        } else {
                            color.write_as_paletted_ansi(&mut output)?;
                        }
                    }
                    write!(output, "{grapheme}")?;
                    position.0 += 1;
                    if position.0 >= wrap_column {
                        position.0 -= wrap_column;
                        position.1 += 1;
                    }
                },

                // Unspecified non-printing character, such as a bell
                // coloring these doesn't make sense
                ConsoleElem::OtherNonPrinting(c) => {
                    write!(output, "{c}")?;
                }

                // Intercept ansi control sequences
                ConsoleElem::Ansi(esc_sequence) => match parse_ansi_type(esc_sequence) {
                    // We don't want the original source to be able to reset our coloring, so
                    // cary out the reset style and then additionally re-apply our color
                    AnsiCodeType::ResetStyle => {
                        write!(output, "{esc_sequence}")?;
                        if config.supports_rgb24 {
                            color.write_as_24bit_ansi(&mut output)?;
                        } else {
                            color.write_as_paletted_ansi(&mut output)?;
                        }
                    }

                    // Simply prevent the original source from changing the color
                    AnsiCodeType::SetColor => {/* discard */},

                    // We allow cursor moves, so long as we can also track them. This way the color
                    // will still match up after a cursor move
                    AnsiCodeType::SetCursor(col, row) => {
                        if let Some(c) = col {
                            position.0 = c;
                        }
                        if let Some(r) = row {
                            position.1 = r
                        }
                        write!(output, "{esc_sequence}")?;
                    },

                    // (See above)
                    AnsiCodeType::MoveCursor(col, row) => {
                        if let Some(d) = col {
                            position.0 = if d > 0 {
                                position.0.saturating_add(d as usize)
                            } else {
                                position.0.saturating_sub(d as usize)
                            };
                        }
                        if let Some(d) = row {
                            position.1 = if d > 0 {
                                position.1.saturating_add(d as usize)
                            } else {
                                position.1.saturating_sub(d as usize)
                            };
                        }
                        write!(output, "{esc_sequence}")?;
                    }

                    // Ideally we'd also handle codes which move already printed characters,
                    // but in doing so we'd need to track the entire terminal screen ourselves.

                    // Forward any other control sequence, hoping that it doesn't cause us any
                    // issues
                    _ => {
                        write!(output, "{esc_sequence}")?;
                    },
                },

                // Some raw binary data - not valid utf-8. Just send it on, and hope that
                // the destination knows what to do with it.
                ConsoleElem::NonUTF8Data(b) => {
                    output.write_all(&[b])?;
                }
            }

            Ok(())
        })
    }
}



pub struct Noop;

impl StreamColorizer for Noop {
    fn copy_colorized<I, O>(&mut self, mut input: I, mut output: O, _: &ColorizerConfig) -> io::Result<()>
        where I: Read, O: Write {
        copy(&mut input, &mut output).map(|_| ())
    }
}


/// Positional colorizer that creates stripes of colors, resembling a striped flag
pub struct Flag {
    pub hf: f32,
    pub vf: f32,
    pub stripes: Vec<Color>,
    pub deadzone: f32,
}


impl PositionalRecolorizer for Flag {
    fn get_color(&mut self, (x, y): (usize, usize)) -> Color {
        let d = (x as f32) * self.hf + (y as f32) * self.vf;

        let base_index = d as usize;
        let frac = d.fract();

        let frac = ((frac - self.deadzone) / (1f32 - self.deadzone)).clamp(0f32, 1f32);

        let real_index = base_index % self.stripes.len();
        let next_index = if real_index + 1 == self.stripes.len() {
            0
        } else {
            real_index + 1
        };

        let col_a = self.stripes[real_index];
        let col_b = self.stripes[next_index];

        col_a.rgb_interpolate(col_b, frac)
    }
}


/// Positional colorizer that uses a reference image =
pub struct Image<T> {
    img: T,
}


impl<T> Image<T> {
    pub fn new(img: T) -> Self {
        Self {
            img,
        }
    }
}


impl<T> PositionalRecolorizer for Image<T>
    where T: GenericImageView,
          <<T as GenericImageView>::Pixel as Pixel>::Subpixel: Into<u8> {
    fn get_color(&mut self, (x, y): (usize, usize)) -> Color {
        let pixel = self.img.get_pixel(
            x as u32 % self.img.width(),
            y as u32 % self.img.height());

        let Rgb([r, g, b]) = pixel.to_rgb();
        Color::from_rgb(r.into(), g.into(), b.into())
    }
}
