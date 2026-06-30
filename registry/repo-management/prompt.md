When adding a third-party APT repository, do it the modern way:

1. Download the repository's signing key with curl.
2. Store it under /etc/apt/keyrings (create the directory if needed).
3. Reference the key with a signed-by= option in the sources list entry.
4. Run apt-get update.

Never pipe a key straight into apt-key; it has been deprecated for
years and the agent that does it anyway will be reverted.
