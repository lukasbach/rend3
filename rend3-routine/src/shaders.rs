//! Holds the shader processing infrastructure for all shaders.
use rend3::ShaderPreProcessor;
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/shaders/src"]
struct Rend3RoutineShaderSources;

pub fn builtin_shaders(spp: &mut ShaderPreProcessor) {
    spp.add_shaders_embed::<Rend3RoutineShaderSources>("rend3-routine");
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use codespan_reporting::{
        diagnostic::{Diagnostic, Label},
        files::SimpleFile,
        term::{
            self,
            termcolor::{ColorChoice, StandardStream},
        },
    };
    use naga::WithSpan;
    use rend3::{RendererProfile, ShaderConfig, ShaderPreProcessor};

    use crate::shaders::Rend3RoutineShaderSources;

    fn print_err(error: &dyn Error) {
        eprint!("{}", error);

        let mut e = error.source();
        if e.is_some() {
            eprintln!(": ");
        } else {
            eprintln!();
        }

        while let Some(source) = e {
            eprintln!("\t{}", source);
            e = source.source();
        }
    }

    pub fn emit_annotated_error<E: Error>(ann_err: &WithSpan<E>, filename: &str, source: &str) {
        let files = SimpleFile::new(filename, source);
        let config = codespan_reporting::term::Config::default();
        let writer = StandardStream::stderr(ColorChoice::Auto);

        let diagnostic = Diagnostic::error().with_labels(
            ann_err
                .spans()
                .map(|(span, desc)| Label::primary((), span.to_range().unwrap()).with_message(desc.to_owned()))
                .collect(),
        );

        term::emit(&mut writer.lock(), &config, &files, &diagnostic).expect("cannot write error");
    }

    #[test]
    fn validate_inherent_shaders() {
        let mut pp = ShaderPreProcessor::new();
        pp.add_shaders_embed::<Rend3RoutineShaderSources>("rend3-routine");

        for shader in pp.files() {
            if !shader.contains(".wgsl") {
                continue;
            }

            let configs = if pp.get(&*shader).unwrap().contains("#if") {
                &[
                    ShaderConfig {
                        profile: Some(RendererProfile::CpuDriven),
                    },
                    ShaderConfig {
                        profile: Some(RendererProfile::GpuDriven),
                    },
                ][..]
            } else {
                &[ShaderConfig { profile: None }][..]
            };

            for config in configs {
                let serialized_config = serde_json::to_value(config).unwrap();
                println!("Testing shader {shader} with config {serialized_config:?}");

                let output = pp.render_shader(&shader, config);

                assert!(output.is_ok(), "Expected preprocessing success, got {output:?}");
                let output = output.unwrap_or_else(|e| panic!("Expected preprocessing success, got {e:?}"));

                let sm = match naga::front::wgsl::parse_str(&output) {
                    Ok(m) => m,
                    Err(e) => {
                        e.emit_to_stderr_with_path(&output, &shader);
                        panic!();
                    }
                };

                let mut validator =
                    naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all());

                match validator.validate(&sm) {
                    Ok(_) => {}
                    Err(err) => {
                        emit_annotated_error(&err, &shader, &output);
                        print_err(&err);
                        panic!()
                    }
                };
            }
        }
    }
}
