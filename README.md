# PrettyCat
PrettyCat is a [queercat](https://github.com/Elsa002/queercat)/[lolcat](https://github.com/busyloop/lolcat) clone, written in rust.

Works like `cat`, but adds its own pretty colors to the console output, such as pride flags!

### Flags
Stipes of color can be based on a flag
Various presets are included (see `prettycat --presets` for a full list!) and custom stripe patterns are supported using `prettycat --custom`.

![image](https://github.com/user-attachments/assets/138e0ac8-0221-4799-8ba3-34a21cdf0cbe)


### Image
Alternatively, images can be used, with automatic resizing support. Here an exact height is given, since the default allows the image to scroll vertically.

![image](https://github.com/user-attachments/assets/59113400-0120-4e29-990f-d35a727bda67)


## Installation / Building
PrettyCat can be installed system wide using
```
cargo install --path .
```
Otherwise, it can be compiled as a standalone executable usng:
```
cargo build --release
```
