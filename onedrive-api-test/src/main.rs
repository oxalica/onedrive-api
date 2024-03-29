use anyhow::{ensure, Context as _, Result};
use onedrive_api::{Auth, ClientCredential, Permission, Tenant};
use std::{
    env,
    fs::File,
    io::{self, Write as _},
    process::exit,
};

const DEFAULT_REDIRECT_URI: &str = "https://login.microsoftonline.com/common/oauth2/nativeclient";

#[derive(Debug)]
struct Args {
    client_id: String,
    output_file: String,
    redirect_uri: String,
}

fn parse_args() -> Result<Args> {
    let mut args = pico_args::Arguments::from_env();
    let output_file = args
        .opt_value_from_str("-o")?
        .unwrap_or_else(|| ".env".to_owned());
    let redirect_uri = args
        .opt_value_from_str("-r")?
        .unwrap_or_else(|| DEFAULT_REDIRECT_URI.to_owned());
    let client_id = args
        .free_from_str::<String>()
        .context("Missing client id")?;
    ensure!(args.finish().is_empty(), "Too many arguments");
    Ok(Args {
        client_id,
        output_file,
        redirect_uri,
    })
}

fn exit_with_help() -> ! {
    eprintln!(
        "
This binary helps you to get an authorized refresh token to be used in tests.

USAGE: {} [-o <output_file>] [-r <redirect_uri>] <client_id>
    `output_file` is default to be `.env`
    `redirect_url` is default to be `{}`
",
        env::args().next().unwrap(),
        DEFAULT_REDIRECT_URI,
    );
    exit(1);
}

#[rustfmt::skip::macros(writeln)]
#[tokio::main]
async fn main() -> Result<()> {
    let args = parse_args().unwrap_or_else(|err| {
        eprintln!("{err}");
        exit_with_help();
    });

    let auth = Auth::new(
        args.client_id.clone(),
        Permission::new_read().write(true).offline_access(true),
        args.redirect_uri.clone(),
        Tenant::Consumers,
    );
    let url = auth.code_auth_url();
    eprintln!("Code auth url: {url}");
    if open::that(url.as_str()).is_err() {
        eprintln!("Cannot open browser, please open the url above manually.");
    }
    eprintln!("Please login in browser, paste the redirected URL here and then press <Enter>");

    let code = loop {
        const NEEDLE: &str = "nativeclient?code=";

        eprint!("Redirected URL: ");
        io::stdout().flush()?;
        let mut inp = String::new();
        io::stdin().read_line(&mut inp)?;
        let inp = inp.trim();

        match inp.find(NEEDLE) {
            Some(pos) => break inp[pos + NEEDLE.len()..].to_owned(),
            _ => eprintln!("Invalid!"),
        }
    };

    eprintln!("Logining...");
    let token = auth.login_with_code(&code, &ClientCredential::None).await?;
    let refresh_token = token.refresh_token.expect("Missing refresh token");

    {
        let mut f = File::create(&args.output_file)?;
        writeln!(f, "export ONEDRIVE_API_TEST_CLIENT_ID='{}'", args.client_id)?;
        writeln!(f, "export ONEDRIVE_API_TEST_REDIRECT_URI='{}'", args.redirect_uri)?;
        writeln!(f, "export ONEDRIVE_API_TEST_REFRESH_TOKEN='{refresh_token}'")?;
    }

    Ok(())
}
