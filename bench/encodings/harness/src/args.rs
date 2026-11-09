//! The command line of the bench binary.

/// The flags of a run.
#[derive(Debug, Clone, PartialEq)]
pub struct Args {
    pub out: Option<String>,
    pub core_label: String,
    pub duration_s: f64,
    pub paced: bool,
    pub commit: Option<String>,
    pub date: Option<String>,
    pub only: Option<String>,
    pub vary: bool,
}

/// Reads the flags, without the program name.
pub fn parse_args(flags: &[String]) -> Result<Args, String> {
    let mut args = Args {
        out: None,
        core_label: "unlabeled".into(),
        duration_s: 0.5,
        paced: false,
        commit: None,
        date: None,
        only: None,
        vary: false,
    };
    let mut it = flags.iter().cloned();
    while let Some(flag) = it.next() {
        let mut value = |name: &str| it.next().ok_or(format!("{name} needs a value"));
        match flag.as_str() {
            "--out" => args.out = Some(value("--out")?),
            "--core-label" => args.core_label = value("--core-label")?,
            "--commit" => args.commit = Some(value("--commit")?),
            "--date" => args.date = Some(value("--date")?),
            "--duration" => {
                args.duration_s = value("--duration")?
                    .parse()
                    .map_err(|e| format!("--duration: {e}"))?;
                if args.duration_s.is_nan() || args.duration_s <= 0.0 {
                    return Err("--duration must be above zero".into());
                }
            }
            "--only" => args.only = Some(value("--only")?),
            "--paced" => args.paced = true,
            "--vary" => args.vary = true,
            other => return Err(format!("unknown flag {other}")),
        }
    }
    if args.paced && args.out.is_none() {
        return Err("--paced needs --out".into());
    }
    if args.vary && args.out.is_none() {
        return Err("--vary needs --out".into());
    }
    Ok(args)
}
