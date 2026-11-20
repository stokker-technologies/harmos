/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Serve One Plugin Operation

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::env;

// Third Party Imports
use tokio::runtime::Builder;

// Local Imports
use harmos::plugin::{Context, HostOperation, Operation, Plugin};
use harmos::{Error, Message, Result, SettingDeclaration, SettingValue, SettingsDeclarations, Visibility, setting_value};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Message)]
struct EchoRequest {
    text: String,
}

#[derive(Message)]
struct EchoResponse {
    text: String,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

fn main() -> Result<()> {
    let settings = SettingsDeclarations {
        values: vec![SettingDeclaration {
            name: "prefix".into(),
            title: "Prefix".into(),
            description: "Text before each answer".into(),
            default: Some(SettingValue {
                value: Some(setting_value::Value::Text("echo: ".into())),
            }),
        }],
    };
    let plugin = Plugin::new("echo", "1.0.0")
        .register(|ctx| {
            ctx.register(settings)?;
            ctx.register(HostOperation::PluginsSettingsList)?;
            ctx.register(Operation::new("example.echo", echo).visibility(Visibility::Public))?;
            Ok(())
        })
        .expect("Register Plugin Declarations");
    if env::var_os("HARMOS_ADDRESS").is_some() {
        return plugin.run();
    }
    let runtime = Builder::new_current_thread().enable_all().build()?;
    runtime.block_on(async move {
        let mut listener = plugin.remote().start("127.0.0.1:0").await?;
        println!("Listening on {}", listener.address());
        listener.wait().await
    })
}

/* === Private Functions === */

async fn echo(context: &Context, request: EchoRequest) -> Result<EchoResponse> {
    let settings = context.settings().list().await?.value.ok_or_else(|| Error::Invalid("Echo: Settings Missing".into()))?;
    let prefix = match settings.values.get("prefix").and_then(|setting| setting.value.as_ref()) {
        Some(setting_value::Value::Text(prefix)) => prefix,
        _ => return Err(Error::Invalid("Echo: Text Prefix Required".into())),
    };
    Ok(EchoResponse {
        text: format!("{prefix}{}", request.text),
    })
}

/* === Tests === */
