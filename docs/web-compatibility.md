# Web compatibility matrix

Record the candidate SHA, browser build, WebKitGTK version, installed package
and display backend before running this checklist. The first column names
the behavior users rely on. The automated column describes reproducible
loopback fixtures in [daily-smoke.py](../scripts/daily-smoke.py).

| Scenario | Automated evidence | Native release check |
| --- | --- | --- |
| Forms and redirects | Required field, form-encoded POST with spaces and an ampersand, then a 303 confirmation page recorded in history | Submit a form on a public test/preview service with synthetic values |
| Authentication pop-ups | A related tab retains `window.opener`, exchanges a message and closes itself | Open a sign-in or consent pop-up with a dedicated test account and complete the return flow |
| Uploads and downloads | Upload a local disposable file, save to a chosen destination and verify the recorded download | Repeat with the Flatpak portal and open the downloaded fixture |
| Notifications | Permission prompt, persistence after reload and page observation of the granted state | Grant, deny and revoke on a test origin, then confirm the site sees the changes |
| Video and audio | Manual coverage only | Play, seek and enter full screen in a public media sample. Record codecs and formats tested. |
| Video calls | Manual coverage only | Join a call in a disposable room and exercise camera/mic consent, mute and disconnect |

Record one result per package/display pair from the
[support contract](support.md). Include the fixture or public site and its
version or check date. Mark an unrun or unsupported scenario explicitly.
Keep personal browsing profiles, accounts and credentials out of release
evidence. Use the [issue reporting guidance](../CONTRIBUTING.md) to file a
reproducible result.
