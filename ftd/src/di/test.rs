use {indoc::indoc, pretty_assertions::assert_eq}; // macro

#[track_caller]
fn p(s: &str, t: &Vec<ftd::di::DI>) {
    let sections = ftd::p11::parse(s, "foo").unwrap_or_else(|e| panic!("{:?}", e));
    let ast = ftd::di::DI::from_sections(sections.as_slice(), "foo")
        .unwrap_or_else(|e| panic!("{:?}", e));
    assert_eq!(t, &ast,)
}

#[track_caller]
fn f(s: &str, m: &str) {
    let sections = ftd::p11::parse(s, "foo").unwrap_or_else(|e| panic!("{:?}", e));
    let ast = ftd::di::DI::from_sections(sections.as_slice(), "foo");
    match ast {
        Ok(r) => panic!("expected failure, found: {:?}", r),
        Err(e) => {
            let expected = m.trim();
            let f2 = e.to_string();
            let found = f2.trim();
            if expected != found {
                let patch = diffy::create_patch(expected, found);
                let f = diffy::PatchFormatter::new().with_color();
                print!(
                    "{}",
                    f.fmt_patch(&patch)
                        .to_string()
                        .replace("\\ No newline at end of file", "")
                );
                println!("expected:\n{}\nfound:\n{}\n", expected, f2);
                panic!("test failed")
            }
        }
    }
}

#[test]
fn test_all() {
    // we are storing files in folder named `t` and not inside `tests`, because `cargo test`
    // re-compiles the crate and we don't want to recompile the crate for every test
    for (files, json) in find_file_groups() {
        let t: Vec<ftd::di::DI> =
            serde_json::from_str(std::fs::read_to_string(json).unwrap().as_str()).unwrap();
        for f in files {
            let s = std::fs::read_to_string(&f).unwrap();
            println!("testing {}", f.display());
            p(&s, &t);
        }
    }
}

fn find_all_files_matching_extension_recursively(
    dir: impl AsRef<std::path::Path>,
    extension: &str,
) -> Vec<std::path::PathBuf> {
    let mut files = vec![];
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            files.extend(find_all_files_matching_extension_recursively(
                &path, extension,
            ));
        } else {
            match path.extension() {
                Some(ext) if ext == extension => files.push(path),
                _ => continue,
            }
        }
    }
    files
}

fn find_file_groups() -> Vec<(Vec<std::path::PathBuf>, std::path::PathBuf)> {
    let files = {
        let mut f = find_all_files_matching_extension_recursively("src/di/t", "ftd");
        f.sort();
        f
    };

    let mut o: Vec<(Vec<std::path::PathBuf>, std::path::PathBuf)> = vec![];

    for f in files {
        let json = filename_with_second_last_extension_replaced_with_json(&f);
        match o.last_mut() {
            Some((v, j)) if j == &json => v.push(f),
            _ => o.push((vec![f], json)),
        }
    }

    o
}

fn filename_with_second_last_extension_replaced_with_json(
    path: &std::path::Path,
) -> std::path::PathBuf {
    let stem = path.file_stem().unwrap().to_str().unwrap();

    path.with_file_name(format!(
        "{}.json",
        match stem.split_once('.') {
            Some((b, _)) => b,
            None => stem,
        }
    ))
}

#[test]
fn di_import() {
    p(
        "-- import: foo",
        &ftd::di::DI::Import(ftd::di::Import {
            module: "foo".to_string(),
            alias: None,
        })
        .list(),
    );

    p(
        "-- import: foo as f",
        &vec![ftd::di::DI::Import(ftd::di::Import {
            module: "foo".to_string(),
            alias: Some("f".to_string()),
        })],
    );

    f(
        "-- import:",
        "ASTParseError: foo:1 -> Expected value in caption for import statement, found: `None`",
    );

    f(
        indoc!(
            "
            -- import: foo

            -- ftd.text: Hello

            -- end: import
            "
        ),
        "ASTParseError: foo:1 -> Subsection not expected for import statement `Section { name: \"import\", \
        kind: None, caption: Some(KV(KV { line_number: 1, key: \"$caption$\", kind: None, value: Some(\"foo\"), \
        condition: None })), headers: Headers([]), body: None, sub_sections: [Section { name: \"ftd.text\", \
        kind: None, caption: Some(KV(KV { line_number: 3, key: \"$caption$\", kind: None, value: Some(\"Hello\"), \
        condition: None })), headers: Headers([]), body: None, sub_sections: [], is_commented: false, line_number: 3, \
        block_body: false }], is_commented: false, line_number: 1, block_body: false }`",
    )
}

#[test]
fn di_record() {
    p(
        indoc!(
            "
            -- record foo:
            string name:
            integer age: 40
            "
        ),
        &ftd::di::DI::Record(
            ftd::di::Record::new("foo")
                .add_field("name", "string", None)
                .add_field("age", "integer", Some(s("40"))),
        )
        .list(),
    );

    p(
        indoc!(
            "
            -- record foo:
            integer age:

            -- string foo.details:

            This contains details for record `foo`.
            This is default text for the field details.
            It can be overridden by the variable of this type.
            "
        ),
        &ftd::di::DI::Record(
            ftd::di::Record::new("foo")
                .add_field("age", "integer", None)
                .add_field(
                    "details",
                    "string",
                    Some(s(indoc!(
                        "This contains details for record `foo`.
                        This is default text for the field details.
                        It can be overridden by the variable of this type."
                    ))),
                ),
        )
        .list(),
    );

    f(
        indoc!(
            "
            -- record foo:
            string name:
            age:
            "
        ),
        "ASTParseError: foo:3 -> Can't find kind for record field: `\"age\"`",
    );
}

#[test]
fn di_variable_definition() {
    p(
        indoc!(
            "
            -- string about-us:

            FifthTry is Open Source

            Our suite of products are open source and available on Github. You are free to download 
            install and customize them to your needs.

            We’d love to hear your feedback and suggestions, and collectively make Documentation 
            easier and better for everyone.
            "
        ),
        &ftd::di::DI::Definition(
            ftd::di::Definition::new("about-us", "string")
            .add_body(indoc!(
                "FifthTry is Open Source

                Our suite of products are open source and available on Github. You are free to download 
                install and customize them to your needs.
    
                We’d love to hear your feedback and suggestions, and collectively make Documentation 
                easier and better for everyone."
            )),
        ).list(),
    );

    p(
        "-- string about-us: FifthTry is Open Source",
        &ftd::di::DI::Definition(
            ftd::di::Definition::new("about-us", "string")
                .add_caption_str("FifthTry is Open Source"),
        )
        .list(),
    );

    p(
        "-- string list names:",
        &ftd::di::DI::Definition(ftd::di::Definition::new("names", "string list")).list(),
    );
}

#[test]
fn di_component_definition() {
    // let v = ftd::di::DI::Definition(
    //     ftd::di::Definition::new("markdown", "ftd.text")
    //         .add_value_property("text", Some(s("caption or body")), None)
    //         .add_value_property("text", None, Some(s("$text"))),
    // )
    // .list();
    // dbg!(serde_json::to_string(&v));
    p(
        indoc!(
            "
            -- ftd.text markdown:
            caption or body text:
            text: $text
            "
        ),
        &ftd::di::DI::Definition(
            ftd::di::Definition::new("markdown", "ftd.text")
                .add_value_property("text", Some(s("caption or body")), None, None)
                .add_value_property("text", None, Some(s("$text")), None),
        )
        .list(),
    );

    p(
        indoc!(
            "
            -- ftd.text markdown:
    
            -- caption or body markdown.text:
    
            -- markdown.text: $text
            "
        ),
        &ftd::di::DI::Definition(
            ftd::di::Definition::new("markdown", "ftd.text")
                .add_value_property("text", Some(s("caption or body")), None, None)
                .add_value_property("text", None, Some(s("$text")), None),
        )
        .list(),
    );

    p(
        indoc!(
            "
            -- ftd.column foo:
            
            -- ftd.ui foo.bar:

            -- ftd.text: Hello there

            -- end: foo.bar
            
            -- bar:

            -- end: foo
            "
        ),
        &ftd::di::DI::Definition(
            ftd::di::Definition::new("foo", "ftd.column")
                .add_di_property(
                    "bar",
                    Some(s("ftd.ui")),
                    ftd::di::DI::Invocation(
                        ftd::di::Invocation::new("ftd.text").add_caption_str("Hello there"),
                    )
                    .list(),
                )
                .add_child(ftd::di::DI::Invocation(ftd::di::Invocation::new("bar"))),
        )
        .list(),
    );
}

#[test]
fn di_variable_invocation() {
    p(
        indoc!(
            "
            -- about-us:

            FifthTry is Open Source

            Our suite of products are open source and available on Github. You are free to download 
            install and customize them to your needs.

            We’d love to hear your feedback and suggestions, and collectively make Documentation 
            easier and better for everyone.
            "
        ),
        &ftd::di::DI::Invocation(
            ftd::di::Invocation::new("about-us")
                .add_body(indoc!(
                "FifthTry is Open Source

                Our suite of products are open source and available on Github. You are free to download 
                install and customize them to your needs.
    
                We’d love to hear your feedback and suggestions, and collectively make Documentation 
                easier and better for everyone."
            )),
        ).list(),
    );

    p(
        "-- about-us: FifthTry is Open Source",
        &ftd::di::DI::Invocation(
            ftd::di::Invocation::new("about-us").add_caption_str("FifthTry is Open Source"),
        )
        .list(),
    );

    p(
        "-- names:",
        &ftd::di::DI::Invocation(ftd::di::Invocation::new("names")).list(),
    );
}

#[test]
fn di_component_invocation() {
    p(
        indoc!(
            "
            -- markdown:
            caption or body text:
            text: $text
            "
        ),
        &ftd::di::DI::Invocation(
            ftd::di::Invocation::new("markdown")
                .add_value_property("text", Some(s("caption or body")), None)
                .add_value_property("text", None, Some(s("$text"))),
        )
        .list(),
    );

    p(
        indoc!(
            "
            -- markdown:
    
            -- caption or body markdown.text:
    
            -- markdown.text: $text
            "
        ),
        &ftd::di::DI::Invocation(
            ftd::di::Invocation::new("markdown")
                .add_value_property("text", Some(s("caption or body")), None)
                .add_value_property("text", None, Some(s("$text"))),
        )
        .list(),
    );

    p(
        indoc!(
            "
            -- foo:
            
            -- foo.bar:

            -- ftd.text: Hello there

            -- end: foo.bar
            
            -- bar:

            -- end: foo
            "
        ),
        &ftd::di::DI::Invocation(
            ftd::di::Invocation::new("foo")
                .add_di_property(
                    "bar",
                    None,
                    ftd::di::DI::Invocation(
                        ftd::di::Invocation::new("ftd.text").add_caption_str("Hello there"),
                    )
                    .list(),
                )
                .add_child(ftd::di::DI::Invocation(ftd::di::Invocation::new("bar"))),
        )
        .list(),
    );
}

fn s(s: &str) -> String {
    s.to_string()
}