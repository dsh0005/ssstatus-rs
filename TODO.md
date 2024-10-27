Big list of TODO:

1. Change the whole `Arc<Mutex<StatusbarData>>` thing to use `Sender`
   and `Receiver`s.
2. Change all the `println!`s to `write!` to some configurable FD.
3. Subscribe to events from UPower for battery changes.
4. Subscribe to events for timezone changes.
5. Hook up the uh... everything.
