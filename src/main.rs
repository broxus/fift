use std::error::Error;
use std::process::ExitCode;

fn main() -> Result<ExitCode, Box<dyn Error>> {
    let mut stdin = std::io::stdin().lock();
    let mut stdout = std::io::stdout();

    let mut ctx = fift::Context::new(&mut stdin, &mut stdout).with_basic_modules()?;

    let exit_code = ctx.run()?;

    Ok(ExitCode::from(!exit_code))
}
