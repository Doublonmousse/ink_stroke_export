This small rust program takes the output of `interactive-ink-examples-uwp` and converts it to rnote file, finalising the conversion from nebo to rnote.

Supported
- handwritten strokes
- images
- lines

There are some limitations though, arcs and glyphs are not read (in practice this means text is not converted, as having text separated by character is a little cumbersome to deal with).