# Security

Please do not report a security problem in a public issue or pull request.

Use GitHub's private reporting: open the repository's **Security** tab and choose **Report a vulnerability**. Include what you found, how to reproduce it, and which version it affects. Do not include working exploits for other people's systems.

Brachio runs on your own machine with your own agent accounts. The terminal runtime listens only on a local endpoint that your user can open (an owner-only named pipe on Windows, a socket file in a private folder elsewhere). A report that shows another user or a remote machine reaching it is in scope.
