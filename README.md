# textifyopendocument

A fast converter from OpenDocument files (text, spreadsheets, presentations)
to plain text, designed with file-manager previews in mind.

This project began as a speed-focused reimagining of
[odt2txt](https://github.com/dstosberg/odt2txt) by Dennis Stosberg. The
original processes OpenDocument files with regular expressions, which becomes
extremely slow on large documents. textifyopendocument replaces the
regular-expression pipeline with streaming XML parsing.

## Licence

GPL, version 2. See LICENSE.

## Acknowledgements

* Dennis Stosberg, author of odt2txt, whose tool and his advice to replace
  the regular-expression approach with XML parsing inspired this project.
