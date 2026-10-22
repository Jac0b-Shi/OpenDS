# Signed tag releases

The release workflow follows BetterGI-mac's owner-signed tag gate. Only signed
annotated `v*` tags made with primary key
`B801AC543CB5B2BBF9407DB2EDDF1F5FEF954168` (or its signing subkey) are accepted.
Public keys are downloaded from the owner's GitHub profile and matched against
that pinned fingerprint. No private signing key or PAT is stored in Actions.

Before releasing, update `Cargo.toml` and `Cargo.lock` together to the intended
version, complete review, and push the signed commit to `main`. The tag version
must exactly match `Cargo.toml`, including any prerelease suffix. For example,
after preparing version `0.1.0`:

```powershell
git tag -s v0.1.0 -m "OpenDS 0.1.0"
git push origin refs/tags/v0.1.0
```

The workflow checks the signature, tag name, triggering commit, main ancestry
and package version. It builds the verified commit through the same Windows
and VIIPER tests used by normal CI. A final job with release write permission
downloads those artifacts without checking out or executing repository code.
It checks that the remote tag still matches the verified object before upload
and again before making the release visible.

Release assets are the Windows x64 binary ZIP, matching source ZIP,
`SHA256SUMS.txt` and `TAG-SIGNATURE.json`. A version with a prerelease suffix
creates a GitHub prerelease. Uploads first create a draft; only a complete
upload is published. Existing releases are not silently overwritten. If a run
fails after creating a draft, inspect and remove that incomplete draft before
rerunning the workflow. Do not move a published release tag.

The signature authenticates the release tag; it does not add Windows
Authenticode signing to the executable. Publishing a release does not change
the repository's private/public visibility. No initial release tag is created
by installing this workflow.
