<h1 style="display: flex; align-items: center;">
  <img src="icon.svg" alt="Icon" style="margin-right: 8px; width: 32px; height: 32px;">
  Redirector
</h1>

A simple tool to redirect web searches using DuckDuckGo bangs.

Redirector is a lightweight web server and command line utility written in Rust that lets you quickly redirect your search queries based on DDG bang syntax.

## Features

- **DDG Bang Support:** Automatically detects and handles search queries using DuckDuckGo bangs (e.g., `!g` for Google, `!w` for Wikipedia).
- **Fast & Lightweight:** Built in Rust for efficient performance and low resource consumption.
- **Easy to Use:** Minimal setup required – simply run the server and start searching.
- **Extendable:** Designed to be a simple base for further customization or integration into your own projects.

## Installation

### Prerequisites

- [Rust](https://www.rust-lang.org/) and Cargo must be installed on your system.

### Building from Source

Clone the repository:

```bash
git clone https://github.com/adolar0042/redirector.git
cd redirector
```

Build the project in release mode:

```bash
cargo build --release
# or install
cargo install --path .
```

## Usage

Run the executable, that's it. It will act as a web server.
Visit the IP and port you set (or if you haven't the default 127.0.0.1:3000), if the program is running this will redirect you to `/bangs`, a list of all loaded bangs.
At this point you can usually right-click the address bar and add Redirector as a search engine.

Redirector can also resolve queries directly from the command line. For example, if you want to search for "Rust programming language" using Google, you can use the following command:

```bash
redirector resolve '!g Rust programming language'
```

This command processes your query and returns the result to standard output.

## Configuration

When started, redirector looks in `~/.config/redirector` for a `config.toml` with the following format:

```toml
ip = "127.0.0.1"
port = 3000
bangs_url = "https://duckduckgo.com/bang.js"
default_search = "https://www.qwant.com/?q={}"
search_suggestions = "https://search.brave.com/api/suggest?q={}" # alternatively you can also use Qwant: https://api.qwant.com/v3/suggest/?q={}&client=opensearch

[[bangs]] # this scheme can be repeated multiple times
category = "Entertainment"                           # currently unused, possible values: Entertainment, Multimedia, News, OnlineServices, Research, Shopping, Tech, Translation
domain = "http://127.0.0.1/bangs"
relevance = 0                                        # currently unused
short_name = "Bangs Page"                            # currently unused
subcategory = "Fun stuff"                            # currently unused
trigger = "bang"
url_template = "http://127.0.0.1/bangs?parameter={{{s}}}" # {{{s}}} gets replaced with the search term
```

## License

This project is licensed under the [GPLv3 License](LICENSE). See the LICENSE file for more information.
