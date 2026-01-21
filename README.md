# ssg - my personal static site generator

This is a static site generator built to build my own personal site. 
Customisability, configurability, and extensibility were not considered in its
design; it's built with me as the sole user. Feel free to use it or fork it 
regardless, but your mileage may vary.

## What it does

This is a simple static site generator. It's meant  to

- translate Markdown to HTML,
- highlight code block syntax,
- optionally append some shared footer, CSS styling, _etc._ to the generated 
  HTML, and
- generate an RSS feed

## What it does not do

`ssg` does not

- do any form of templating,
- take in any sort of configuration file or command line arguments.

## How it works

`ssg` assumes two things, namely that

- all contents are placed in a directory called `./contents/`, and
- your stylesheet is a file called `./style.css`.

Simply run `ssg`, and it will compile it into a page in `./site/`
