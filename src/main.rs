use std::fs;
use std::io::{BufReader, BufWriter, Read, stderr, stdin, stdout, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use anyhow::{anyhow, Result};

use clap::{Args, Parser};
use image::{open, RgbImage};
use image::imageops::{FilterType, resize};

use crate::console::RESET_CODE;
use crate::console::Color;
use crate::presets::{default_flag_preset, flag_by_name, iter_flag_presets};
use crate::stream_colors::{ColorizerConfig, Flag, Image, Noop, StreamColorizer};

mod stream_colors;
mod console;
mod presets;


#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Opt {
    #[clap(flatten)]
    colorizer: ColorizerOpts,

    /// A sequence of files to read, in order. "-" represents stdin
    #[arg(default_values = ["-"])]
    files: Vec<PathBuf>,

    /// Disallow the use of 24-bit rgb ANSI codes. This may improve support on terminals
    /// that don't support these codes. NOTE: Color reproduction is very poor at the moment!
    #[arg(short, long)]
    disable_rgb24: bool,

    /// Override terminal width with the given value
    #[arg(short, long)]
    width_override: Option<usize>,
}


/// This struct encapsulates all the arguments for each colorizer.
#[derive(Debug, Args)]
#[group(required = false)]
struct ColorizerOpts {
    #[clap(flatten)]
    noop: NoopOpts,

    #[clap(flatten)]
    flag: FlagOpts,

    #[clap(flatten)]
    image: ImageOpts,
}


impl ColorizerOpts {
    /// Check for early-exit behaviour, such as displaying all available presets, and perform it if
    /// possible. Returns Ok(false) if no such behaviour is possible, otherwise Ok(true) or any error
    /// is returned.
    fn try_early_exit(&self) -> Result<bool> {
        self.flag.maybe_print_presets()
    }

    /// Convert to a [SomeColorizer] instance.
    ///
    /// Config is *not* passed to the colorizer, this must happen when calling copy_colorized.
    /// Instead, config is used to prepare certain resources such as resizing images beforehand
    fn try_into_colorizer(self, config: &ColorizerConfig) -> Result<SomeColorizer> {
        self.noop.into_colorizer()
            .or(self.flag.into_colorizer())
            .or(self.image.into_colorizer(config))

            .unwrap_or_else(|| {
                Ok(SomeColorizer::Flag(Flag {
                    hf: 0.05,
                    vf: 0.05,
                    stripes: default_flag_preset().stripes.to_vec(),
                    deadzone: 0.6,
                }))
            })
    }
}


/// Options for the No-op colorizer
#[derive(Debug, Args)]
struct NoopOpts {
    /// Output without colorizing - equivalent to cat
    #[arg(long)]
    noop: bool,
}


impl NoopOpts {
    fn into_colorizer(self) -> Option<Result<SomeColorizer>> {
        if self.noop {
            Some(Ok(SomeColorizer::Noop(Noop)))
        } else {
            None
        }
    }
}


/// Options for the striped flag colorizer
#[derive(Debug, Args)]
struct FlagOpts {
    /// Output a flag from a preset. View all presets using --presets
    #[arg(long)]
    flag: Option<String>,

    /// List all preset flags
    #[arg(long)]
    presets: bool,

    /// Use a custom comma seperated sequence of colours to form a striped flag. Colors can be
    /// specified using hex codes
    #[arg(long, value_delimiter=',')]
    custom: Option<Vec<Color>>,

    /// Horizontal frequency, in stripes/column
    #[arg(long, default_value="0.05")]
    hf: f32,

    /// Vertical flag frequency, in stripes/row
    #[arg(long, default_value="0.05")]
    vf: f32,

    /// Fraction of a stripe after reaching a new stripe before beginning to blend into the next
    #[arg(long, default_value="0.6")]
    deadzone: f32
}


impl FlagOpts {
    /// Print presets if appropriate, otherwise return Ok(false)
    fn maybe_print_presets(&self) -> Result<bool> {
        if self.presets {
            let mut stdout = stdout().lock();

            let longest_name = iter_flag_presets()
                .map(|flag| flag.name.len())
                .max()
                .unwrap_or_default();

            for flag in iter_flag_presets() {
                // Print name
                write!(stdout, "{:<1$} | ", flag.name, longest_name)?;

                // Print each stripe, in its color
                for (i, stripe) in flag.stripes.iter().enumerate() {
                    stripe.write_as_24bit_ansi(&mut stdout)?;
                    write!(stdout, "{stripe}{RESET_CODE}")?;
                    if i < flag.stripes.len()-1 {
                        write!(stdout, ",")?;
                    }
                }

                write!(stdout, "\n")?;
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn into_colorizer(self) -> Option<Result<SomeColorizer>> {
        // First check if a preset name has been given
        if let Some(name) = &self.flag {
            // Fetch the preset or return an appropriate error message
            // TODO: use match for clarity?
            let Some(preset) = flag_by_name(name)
                else {
                    return Some(Err(anyhow!("Invalid preset name {name}! - Use --presets to list all available flag presets")));
                };

            let pattern = preset.stripes.to_vec();

            Some(Ok(SomeColorizer::Flag(Flag {
                hf: self.hf,
                vf: self.vf,
                stripes: pattern,
                deadzone: self.deadzone,
            })))
        // Otherwise check if a custom pattern has been given
        } else if let Some(pattern) = self.custom {
            Some(Ok(SomeColorizer::Flag(Flag {
                hf: self.hf,
                vf: self.vf,
                stripes: pattern,
                deadzone: self.deadzone
            })))
        } else {
            None
        }
    }
}


/// Image width, either fixed, the original width, or automatically scaled to the width of the terminal
#[derive(Debug, Clone)]
enum ImageWidth {
    Original,
    Fixed(usize),
    Fit,
}


impl FromStr for ImageWidth {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if s.eq_ignore_ascii_case("fit") {
            Ok(Self::Fit)
        } else if s.eq_ignore_ascii_case("original") {
            Ok(Self::Original)
        } else {
            Ok(Self::Fixed(s.parse()?))
        }
    }
}


/// Height of the image, either fixed, the original height, or automatically derived to maintain the aspect ratio
#[derive(Debug, Clone)]
enum ImageHeight {
    Original,
    Fixed(usize),
    Ratio,
}


impl FromStr for ImageHeight {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if s.eq_ignore_ascii_case("ratio") {
            Ok(Self::Ratio)
        } else if s.eq_ignore_ascii_case("original") {
            Ok(Self::Original)
        } else {
            Ok(Self::Fixed(s.parse()?))
        }
    }
}


/// Options for the image colorizer
#[derive(Debug, Args)]
struct ImageOpts {
    /// Base colors on a provided image
    #[arg(long)]
    image: Option<PathBuf>,

    /// Cell aspect ratio, defined as width/height per cell. Used only when image-height is set to
    /// "ratio"
    #[arg(long, default_value="0.7")]
    cell_aspect_ratio: f64,

    /// Width of the image in pixels, or "fit" to fit the console width
    #[arg(long, default_value="fit")]
    image_width: ImageWidth,

    /// Height of the image in pixels, or "ratio" to maintain the aspect ratio
    #[arg(long, default_value="ratio")]
    image_height: ImageHeight,
}


impl ImageOpts {
    fn into_colorizer(self, config: &ColorizerConfig) -> Option<Result<SomeColorizer>> {
        let path = self.image?;

        let img = match open(path) {
            Ok(img) => img.to_rgb8(),
            Err(e) => return Some(Err(e.into())),
        };

        // Determine width
        let width = match self.image_width {
            ImageWidth::Original => img.width() as usize,  // Yes, it's a bit silly to upcast to usize then back to u32
            ImageWidth::Fixed(x) => x,
            ImageWidth::Fit => config.wraps_after.unwrap_or(80),
        };
        // Convert it to u32, or return an appropriate error
        let width = match width.try_into() {
            Ok(w) => w,
            Err(_) => return Some(Err(anyhow!("Image width {width} is too large!"))),
        };

        // Similar for height
        let height = match self.image_height {
            ImageHeight::Original => img.height() as usize,
            ImageHeight::Fixed(x) => x,
            ImageHeight::Ratio => {
                // Maintain the aspect ratio by copying the same scale factor from the width,
                // taking differing ppc/ppr into account
                let scale_ratio = (width as f64) / (img.width() as f64);
                (img.height() as f64 * scale_ratio * self.cell_aspect_ratio) as usize
            }
        };
        let height = match height.try_into() {
            Ok(h) => h,
            Err(_) => return Some(Err(anyhow!("Image height {height} is too large!"))),
        };

        // Resize
        let img = resize(&img, width, height, FilterType::Gaussian);

        Some(Ok(SomeColorizer::Image(Image::new(img))))
    }
}


/// Enum over stream colorizers, [StreamColorizer] is not object safe.
enum SomeColorizer {
    Noop(Noop),
    Flag(Flag),
    Image(Image<RgbImage>),
}


impl StreamColorizer for SomeColorizer {
    fn copy_colorized<I, O>(&mut self, input: I, output: O, config: &ColorizerConfig) -> std::io::Result<()>
        where I: Read,
              O: Write {
        match self {
            SomeColorizer::Noop(x) => x.copy_colorized(input, output, config),
            SomeColorizer::Flag(x) => x.copy_colorized(input, output, config),
            SomeColorizer::Image(x) => x.copy_colorized(input, output, config),
        }
    }
}



fn open_path(path: impl AsRef<Path>) -> Result<Box<dyn Read>> {
    if path.as_ref() == Path::new("-") {
        Ok(Box::new(stdin().lock()))
    } else {
        match fs::File::open(path.as_ref()) {
            Ok(file) => Ok(Box::new(BufReader::new(file))),
            Err(e) => Err(anyhow!("\"{}\": {e}\n", path.as_ref().display()))
        }
    }
}


fn main() -> Result<()> {
    let args = Opt::parse();

    // Construct colorizer config
    let config = ColorizerConfig {
        wraps_after: args.width_override
            .or_else(|| term_size::dimensions()
                .map(|x| x.0)),

        supports_rgb24: !args.disable_rgb24,

        ..Default::default()
    };

    // Try for early exit before locking stdout (since early exit behavior probably uses it) and
    // before opening input files (since they will never be used)
    if args.colorizer.try_early_exit()? {
        return Ok(());
    }

    // Lock output now, it doesn't need to be relocked repeatedly
    let mut output = BufWriter::new(stdout().lock());

    let input = args.files.iter()
        .map(open_path);

    let mut colorizer = args.colorizer.try_into_colorizer(&config)?;
    for i in input {
        match i {
            Ok(f) => colorizer.copy_colorized(f, &mut output, &config)?,
            Err(e) => {write!(stderr(), "{e}")?},
        }
    }

    Ok(())
}
