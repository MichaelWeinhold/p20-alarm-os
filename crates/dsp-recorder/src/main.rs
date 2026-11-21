use std::process::ExitCode;

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().collect();
    let recorded = dsp_recorder::options(&arguments).and_then(|options| {
        dsp_recorder::linux::record(&options).map_err(|error| error.to_string())
    });
    match recorded {
        Ok(count) => {
            println!("recorded {count} ioctls on {}", dsp_recorder::HIFI_MISC);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("dsp-recorder: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(not(all(target_os = "linux", target_arch = "aarch64")))]
fn main() -> ExitCode {
    eprintln!(
        "dsp-recorder runs only on the phone, under the ROM, where it records {}",
        dsp_recorder::HIFI_MISC
    );
    ExitCode::FAILURE
}
