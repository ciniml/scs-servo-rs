# SCS Servo CLI

## build

Run `cargo build` 

## Usage

### Scan SCS Servo

```shell
scs-servo-cli --port (serial port) [--echo] scan
```

Scan over `/dev/ttyUSB0` (The adapter hardware must discard the TX packet.)

```shell
scs-servo-cli --port /dev/ttyUSB0 scan
```

Scan over `/dev/ttyUSB0` with discarding echo back packet.

```shell
scs-servo-cli --port /dev/ttyUSB0 --echo scan
```

If there is a SCS servo whose ID is 3, the output is like below:

```
$ ./scs-servo-cli --port /dev/ttyUSB0 --echo scan
[2024-05-27T20:09:16Z INFO  scs_servo_cli] Scanning for servos on port /dev/ttyUSB0 at baud rate 1000000
⠉ [00:00:00] [░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░] 2/254 Scanning...                                                                                               [2024-05-27T20:09:16Z INFO  scs_servo_cli] Found servo with ID 3 version 05 04
```

### Read registers

```
scs-servo-cli read --id (id) --address (address) --length (length) [--format (raw|hex)] [--output (path)]
```

e.g. Read Software Version H (0x03), Software Version L (0x04) and the ID (0x05) registers from ID 0x01 SCS servo, in hex string format

```
$ scs-servo-cli read --id 0x01 --address 0x03 --length 3
050401
```

If you want to write the result into a file, specify the file path with `--output` option.

```
$ scs-servo-cli read --id 0x01 --address 0x03 --length 3 --format raw --output output.bin
$ xxd output.bin
00000000: 0504 01
```

### Write registers

```
scs-servo-cli write --id (id) --address (address) [--format (raw|hex)] [--input (path)]
```

e.g. Enable motor torque output (by writing `0x01` to the register 0x28) of ID 0x01 SCS servo.
The data to write is input from STDIN if the `--input` option is not specified.

```
$ echo -n 01 | scs-servo-cli write --id 0x01 --address 0x01 --format hex
```