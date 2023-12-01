Yeah so basically I have a general sense for how I want to calculate the bpm

Take a t second window of data. For now let t = 5 seconds.
Count the number of peaks in this window of data. Let p = the number of peaks
let BPM = p * (60 / t)

BPM should be calculated on the backend and pushed to the front end via tauri

So thats that out of the way : )

As for interfacing with the arduino (these things can be changed but off the top of my head I feel that this would be best):

* Once you plug the arduino into the computer/monitor, the Rust backend will detect that there is new data being sent over the serial bus.

* Once the computer detects that the Arduino is plugged in, the Rust backend will create a new CSV file, and continously write each new received bit of data into the CSV file.
  * ISO Date format should be used
  * Each new entry written should also be sent to the front end where it can be parsed. The CSV entry should have the fully formatted date whereas the object sent to the front-end should have the number of milliseconds.

* Once the computer detects that the arduino has been unplugged, it will finish writing to the file. If the user were to re-plug the arduino into the computer, it would just create a new CSV file.