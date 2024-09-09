fn take_one_argument(remaining: &str, default: isize) -> (&str, isize) {
    if remaining.is_empty() {
        return ("", 0);
    }

    let n = remaining.find(';');
    if let Some(i) = n {
        let next_remaining = &remaining[i..];
        let value = remaining[..i].parse().unwrap_or(default);
        (next_remaining, value)
    } else {
        let value = remaining.parse().unwrap_or(default);
        ("", value)
    }
}


#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum AnsiCodeType {
    ResetStyle,
    SetColor,
    MoveCursor(Option<isize>, Option<isize>),
    SetCursor(Option<usize>, Option<usize>),
    Other,
}


pub fn parse_ansi_type(ansi: &str) -> AnsiCodeType {
    if ansi.len() <= 2 {
        return AnsiCodeType::Other;
    }
    let args = &ansi[2..ansi.len()-1];

    if ansi.ends_with('m') {
        if ansi[1..].starts_with("[3") {
            AnsiCodeType::SetColor
        } else {
            let (_, mode) = take_one_argument(args, 0);
            if mode == 0 {
                AnsiCodeType::ResetStyle
            } else {
                AnsiCodeType::Other
            }
        }

    } else if ansi[1..].ends_with('A') {
        let (_, count) = take_one_argument(args, 1);
        AnsiCodeType::MoveCursor(None, Some(-count))

    } else if ansi[1..].ends_with('B') {
        let (_, count) = take_one_argument(args, 1);
        AnsiCodeType::MoveCursor(None, Some(count))

    } else if ansi[1..].ends_with('C') {
        let (_, count) = take_one_argument(args, 1);
        AnsiCodeType::MoveCursor(Some(count), None)

    } else if ansi[1..].ends_with('D') {
        let (_, count) = take_one_argument(args, 1);
        AnsiCodeType::MoveCursor(Some(-count), None)


    } else if ansi[1..].ends_with('E') {
        let (_, cols) = take_one_argument(args, 1);
        AnsiCodeType::MoveCursor(Some(isize::MIN), Some(cols))

    } else if ansi[1..].ends_with('F') {
        let (_, cols) = take_one_argument(args, 1);
        AnsiCodeType::MoveCursor(Some(isize::MIN), Some(-cols))

    } else if ansi[1..].ends_with('G') {
        let (_, col) = take_one_argument(args, 1);
        AnsiCodeType::SetCursor(Some((col-1) as usize), None)

    } else if ansi[1..].ends_with('H') {
        let (args, row) = take_one_argument(args, 1);
        let (_, col) = take_one_argument(args, 1);
        AnsiCodeType::SetCursor(Some((col-1) as usize), Some((row-1) as usize))

    } else {
        AnsiCodeType::Other
    }
}