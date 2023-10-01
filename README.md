# guardrail

## Todo

- [ ] Database
  - [X] Finish Database trait
  - [ ] Finish tests
  - [X] Add user table
  - [X] Follow annotations/attachments when retrieving crashes
- [ ] API
  - [ ] Error Reporting
  - [ ] Move minidump processing to a separate module
  - [X] Authentication
    - [X] Minidump upload
    - [X] Symbol upload
    - [X] CRUD
  - [X] Implement remaining API endpoints
    - [X] Symbols
    - [X] Crashes (including annotations/attachments)
    - [X] Users
  - [ ] Tests
- Minidump processing
  - [ ] Remove minidump after processing
  - [ ] Periodically clean up left over minidumps
- [ ] Web interface
  - [ ] Authentication
- [ ] Authentication
  - [X] Login
  - [ ] Logout
  - [ ] Roles
- [ ] Misc
  - [ ] Remove unwrap's
- [ ] Infra
  - [X] GitHub action
  - [ ] K8S deployment
  