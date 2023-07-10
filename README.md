# Stembot

This bot is an co-interactive agent whose purpose is to unify separated, heterogenious, and volatile computing environments into a single entity. The bots maintain fault resistent links with one another and continuous advertise their link states and message routing tables with one another. 

Links operate either as bidirectional push-push interactions or unidirectional push-pull interactions. In a push-push link, both bots at the ends of the link actively post to one another. In a push-pull link, one of the bots in a link is both posting outgoing messages and polling for incoming messages from the bot at the far side of the link.

The bots can be addressed individually or globally via broadcast messaging. The bots facilitate broadcast messaging by propogating the broadcast.

## Build and Operation
1. Install Rust via (rustup.sh)[https://rustup.rs]
2. Change directories to the project root.
3. Start the bot 'cargo run -- --config-path etc/config.toml'

## Contributing

1. Fork it!
2. Create your feature branch: `git checkout -b my-new-feature`
3. Commit your changes: `git commit -am 'Add some feature'`
4. Push to the branch: `git push origin my-new-feature`
5. Submit a pull request :D

## History

This project is a rewrite of stembot-python. This project is loosely modelled after
but not message compatible with the python verion of the bot. While this version communicates with AES secured HTTP bodies like the python version, it does not encode the
message payloads as JSON. 

## License

MIT License

Copyright (c) 2023 Justin L. Dierking

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.