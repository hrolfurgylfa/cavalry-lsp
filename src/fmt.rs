use pyo3::{
    intern,
    types::{PyDict, PyModule},
    PyResult, Python,
};
use similar::{utils::TextDiffRemapper, ChangeTag, TextDiff};
use tower_lsp::lsp_types::{Position, Range, TextEdit};

pub fn format_with_black(py: Python<'_>, str: String) -> PyResult<String> {
    let black = PyModule::import(py, "black")?;
    let mode = black.getattr(intern!(py, "Mode"))?.call0()?;

    let kwargs = PyDict::new(py);
    kwargs.set_item("fast", false)?;
    kwargs.set_item("mode", mode)?;

    let report = black.getattr(intern!(py, "report"))?;
    let nothing_changed_err = report.getattr(intern!(py, "NothingChanged"))?;

    match black
        .getattr(intern!(py, "format_file_contents"))?
        .call((&str,), Some(kwargs))
    {
        Ok(formatted) => Ok(formatted.extract::<String>()?),
        Err(e) if e.get_type(py).is_subclass(nothing_changed_err)? => Ok(str.to_owned()),
        Err(e) => Err(e),
    }
}

pub fn format_with_isort(py: Python<'_>, str: String) -> PyResult<String> {
    let isort = PyModule::import(py, "isort")?;

    let kwargs = PyDict::new(py);
    kwargs.set_item("profile", "black")?;
    let config = isort
        .getattr(intern!(py, "Config"))?
        .call((), Some(&kwargs))?;

    let kwargs = PyDict::new(py);
    kwargs.set_item("config", config)?;

    isort
        .getattr(intern!(py, "code"))?
        .call((str,), Some(kwargs))?
        .extract::<String>()
}

pub fn format_in_python(str: String) -> String {
    let formatted: PyResult<_> = Python::with_gil(|py| {
        let sys = PyModule::import(py, "sys")?;
        let executable = sys.getattr(intern!(py, "executable"))?;
        println!("sys: {}", executable.str().unwrap());

        let str = format_with_black(py, str)?;
        let str = format_with_isort(py, str)?;
        Ok(str)
    });

    match formatted {
        Ok(res) => res,
        Err(e) => panic!("Failed to format with black: {}", e),
    }
}

fn count_text(line: &mut u32, column: &mut u32, text: &str) {
    for c in text.bytes() {
        if c == b'\n' {
            *line += 1;
            *column = 0;
        } else {
            *column += 1;
        }
    }
}

pub fn format_to_text_edits(old: &str, new: &str) -> Vec<TextEdit> {
    let diff = TextDiff::from_unicode_words(old, new);
    let remapper = TextDiffRemapper::from_text_diff(&diff, old, new);
    let changes: Vec<_> = diff
        .ops()
        .iter()
        .flat_map(move |x| remapper.iter_slices(x))
        .collect();
    // println!("{}", "=".repeat(50));
    // println!("{:?}", changes);
    // println!("{}", "=".repeat(50));

    let mut edits = vec![];
    let mut next_is_used = false;
    let (mut line, mut column) = (0, 0);
    for i in 0..changes.len() {
        // println!("{}", "-".repeat(50));
        // println!("Curr diff: {:?}", changes[i]);
        if next_is_used {
            next_is_used = false;
            continue;
        }

        let (tag, text) = changes[i];
        let next = changes.get(i + 1);

        match tag {
            ChangeTag::Insert if next.map(|i| i.0) == Some(ChangeTag::Delete) => unreachable!(),
            ChangeTag::Delete => {
                let mut str = "".to_owned();
                if let Some(next) = next
                    && next.0 == ChangeTag::Insert
                {
                    // println!("Extra Insert");
                    next_is_used = true;
                    str = next.1.to_owned();
                }

                // println!("Delete");
                let start = Position::new(line, column);
                count_text(&mut line, &mut column, text);
                let end = Position::new(line, column);
                edits.push(TextEdit {
                    new_text: str,
                    range: Range { start, end },
                });
            }
            ChangeTag::Insert => {
                // println!("Insert");
                let start = Position::new(line, column);
                edits.push(TextEdit {
                    new_text: text.to_owned(),
                    range: Range { start, end: start },
                });
            }
            ChangeTag::Equal => {
                // println!("Just equal...");
                count_text(&mut line, &mut column, text);
            }
        }
    }
    edits
}

#[cfg(test)]
mod tests {
    use tower_lsp::lsp_types::{Position, Range, TextEdit};

    use super::format_to_text_edits;

    #[test]
    fn text_edits_delete_line() {
        let res = format_to_text_edits("AAAApiAAA", "AAAAparAAA");
        assert_eq!(
            res,
            vec![TextEdit {
                new_text: "par".to_owned(),
                range: Range {
                    start: Position::new(0, 4),
                    end: Position::new(0, 6),
                },
            }]
        )
    }
}
