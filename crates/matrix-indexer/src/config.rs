use std::io::Write;

use kdl::{KdlDocument, KdlEntry, KdlIdentifier, KdlNode};
use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

pub struct Config {
    pub homeserver_url: String,
    pub indradb_endpoint: String,
    pub auth_data: AuthData,
}

pub enum AuthData {
    UsernamePassword(String, String),
    AccessToken(String, String, String),
}

#[derive(Debug, Error, Diagnostic)]
#[error("Incorrect Config")]
#[diagnostic()]
pub struct MissingFieldError {
    #[source_code]
    src: NamedSource,

    #[help]
    advice: String,

    #[label]
    snippet: SourceSpan,
}

fn parse(input: &str) -> miette::Result<KdlDocument> {
    Ok(input.parse::<KdlDocument>()?)
}

fn missing_field(source: String, snippet: &SourceSpan, help: &str) {
    let error: miette::Result<()> = Err(MissingFieldError {
        src: NamedSource::new("config.kdl", source),
        snippet: *snippet,
        advice: help.to_string(),
    }
    .into());

    // This is stupid. But oh well.
    if let Err(error) = error {
        eprintln!("{error:?}");
        std::process::exit(1);
    }
}

#[allow(clippy::unwrap_used)]
#[allow(clippy::too_many_lines)]
pub fn load() -> Config {
    let source = std::fs::read_to_string("config.kdl")
        .expect("Unable to open config.kdl file. Is it present?");
    match parse(&source) {
        Ok(config) => {
            let matrix = config.get("matrix");
            if let Some(matrix) = matrix {
                if let Some(matrix_children) = matrix.children() {
                    let homeserver_url_entry = matrix_children.get("homeserver_url");
                    let homeserver_url = if let Some(homeserver_url_entry) = homeserver_url_entry {
                        if let Some(homeserver_url) = homeserver_url_entry.entries().first() {
                            if let Some(homeserver_url) = homeserver_url.value().as_string() {
                                homeserver_url.to_string()
                            } else {
                                missing_field(source, homeserver_url.span(),"\"homeserver_url\" field has incorrect type. Make sure it is actually a string. Try adding quotes.");
                                unreachable!();
                            }
                        } else {
                            missing_field(source, homeserver_url_entry.span(),"\"homeserver_url\" field has no value. Make sure it is actually a string. Try adding quotes.");
                            unreachable!();
                        }
                    } else {
                        missing_field(source,matrix_children.span(),"Missing \"homeserver_url\" field. See the example config for how to define it");
                        unreachable!();
                    };
                    let username_entry = matrix_children.get("username");
                    let username = if let Some(username_entry) = username_entry {
                        if let Some(username) = username_entry.entries().first() {
                            if let Some(username) = username.value().as_string() {
                                username.to_string()
                            } else {
                                missing_field(source, username.span(),"\"username\" field has incorrect type. Make sure it is actually a string. Try adding quotes.");
                                unreachable!();
                            }
                        } else {
                            missing_field(source, username_entry.span(),"\"username\" field has no value. Make sure it is actually a string. Try adding quotes.");
                            unreachable!();
                        }
                    } else {
                        missing_field(source,matrix_children.span(),"Missing \"username\" field. See the example config for how to define it");
                        unreachable!();
                    };
                    let password_entry = matrix_children.get("password");
                    let password = if let Some(password_entry) = password_entry {
                        if let Some(password) = password_entry.entries().first() {
                            if let Some(password) = password.value().as_string() {
                                Some(password.to_string())
                            } else {
                                missing_field(source, password.span(),"\"password\" field has incorrect type. Make sure it is actually a string. Try adding quotes.");
                                unreachable!();
                            }
                        } else {
                            missing_field(source, password_entry.span(),"\"password\" field has no value. Make sure it is actually a string. Try adding quotes.");
                            unreachable!();
                        }
                    } else {
                        None
                    };

                    let indradb_address_entry = config.get("indradb_address");
                    let indradb_endpoint = if let Some(indradb_address_entry) =
                        indradb_address_entry
                    {
                        if let Some(indradb_address) = indradb_address_entry.entries().first() {
                            if let Some(indradb_address) = indradb_address.value().as_string() {
                                indradb_address.to_string()
                            } else {
                                missing_field(source, indradb_address.span(),"\"indradb_address\" field has incorrect type. Make sure it is actually a string. Try adding quotes.");
                                unreachable!();
                            }
                        } else {
                            missing_field(source, indradb_address_entry.span(),"\"indradb_address\" field has no value. Make sure it is actually a string. Try adding quotes.");
                            unreachable!();
                        }
                    } else {
                        missing_field(source,config.span(),"Missing \"indradb_address\" field. See the example config for how to define it");
                        unreachable!();
                    };

                    let access_token_entry = matrix_children.get("access_token");
                    let access_token = if let Some(access_token_entry) = access_token_entry {
                        if let Some(access_token) = access_token_entry.entries().first() {
                            if let Some(access_token) = access_token.value().as_string() {
                                Some(access_token.to_string())
                            } else {
                                missing_field(source, access_token.span(),"\"access_token\" field has incorrect type. Please reset it to be username and password.");
                                unreachable!();
                            }
                        } else {
                            missing_field(source, access_token_entry.span(),"\"access_token\" field has no value. Please reset it to be username and password.");
                            unreachable!();
                        }
                    } else {
                        None
                    };
                    let device_id_entry = matrix_children.get("device_id");
                    let device_id = if let Some(device_id_entry) = device_id_entry {
                        if let Some(device_id) = device_id_entry.entries().first() {
                            if let Some(device_id) = device_id.value().as_string() {
                                Some(device_id.to_string())
                            } else {
                                missing_field(source, device_id.span(),"\"device_id\" field has incorrect type. Please reset it to be username and password.");
                                unreachable!();
                            }
                        } else {
                            missing_field(source, device_id_entry.span(),"\"device_id\" field has no value. Please reset it to be username and password.");
                            unreachable!();
                        }
                    } else {
                        None
                    };

                    let auth_data = if let Some(password) = password {
                        AuthData::UsernamePassword(username, password)
                    } else {
                        // In theory this is safe :D
                        AuthData::AccessToken(username, access_token.unwrap(), device_id.unwrap())
                    };

                    Config {
                        homeserver_url,
                        indradb_endpoint,
                        auth_data,
                    }
                } else {
                    missing_field(
                        source,
                        matrix.span(),
                        "Missing \"matrix\" choldren. See the example config for how to define it",
                    );
                    unreachable!();
                }
            } else {
                missing_field(
                    source,
                    config.span(),
                    "Missing \"matrix\" section. See the example config for how to define it",
                );
                unreachable!();
            }
        }
        Err(error) => {
            eprintln!("{error:?}");
            std::process::exit(1);
        }
    }
}

pub fn write_access_token(access_token: String, device_id: String) -> color_eyre::Result<()> {
    let source = std::fs::read_to_string("config.kdl")
        .expect("Unable to open config.kdl file. Is it present?");
    let config = parse(&source);
    match config {
        Ok(mut config) => {
            let matrix = config.get_mut("matrix");
            if let Some(matrix) = matrix {
                if let Some(matrix_children) = matrix.children_mut() {
                    let matrix_nodes = matrix_children.nodes_mut();
                    let access_token_identifier = KdlIdentifier::from("access_token");
                    let device_id_identifier = KdlIdentifier::from("device_id");

                    let mut access_token_node = KdlNode::new(access_token_identifier);
                    let access_token_entry = KdlEntry::new(access_token);
                    access_token_node.entries_mut().push(access_token_entry);

                    let mut device_id_node = KdlNode::new(device_id_identifier);
                    let device_id_entry = KdlEntry::new(device_id);
                    device_id_node.entries_mut().push(device_id_entry);

                    matrix_nodes.push(access_token_node);
                    matrix_nodes.push(device_id_node);
                    matrix_nodes.retain(|node| node.name().value() != "password");

                    matrix_nodes.sort_by(sort_by_name);
                }
                matrix.fmt();
            }

            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .open("config.kdl")?;

            file.write_all(config.to_string().as_bytes())?;
            file.flush()?;
            Ok(())
        }
        Err(error) => {
            let err = format!("{error:?}");
            Err(color_eyre::eyre::eyre!("{}", err))
        }
    }
}

fn sort_by_name(x: &KdlNode, y: &KdlNode) -> std::cmp::Ordering {
    x.name().value().cmp(y.name().value())
}
