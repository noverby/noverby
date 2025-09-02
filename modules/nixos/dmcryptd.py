#!/usr/bin/python

import socket
import pickle
import subprocess
import os
import uuid
import time

server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
socket_path = "/tmp/socket_dmcrypt.sock"
if os.path.exists(socket_path):
    os.remove(socket_path)
server.bind(socket_path)
os.chmod(socket_path, 438)  # 666

while True:
    server.listen(1)
    conn, addr = server.accept()
    s = socket.fromfd(conn.fileno(), socket.AF_UNIX, socket.SOCK_STREAM)
    size = int.from_bytes(s.recv(4), "big")
    data = pickle.loads(s.recv(size))
    dest_file = data["dest"]
    src_file = data["src"]
    key_file = data["key"]

    print("start")

    try:
        uuid4 = str(uuid.uuid4())
        loop_device = subprocess.check_output(["losetup", "-f"]).decode().strip()
        loop_device_number = loop_device.replace("/dev/loop", "")

        if not os.path.exists(loop_device):
            subprocess.check_output(["mknod", loop_device, "b", "7", loop_device_number])

        subprocess.check_output(["losetup", loop_device, dest_file])
        subprocess.check_output(["cryptsetup", "--key-file", key_file,
                                 "-q", "-y", "-v", "luksFormat", loop_device])
        subprocess.check_output(["cryptsetup", "open", "--key-file",
                                 key_file, loop_device, uuid4])
        subprocess.check_output(["dd", "if=" + src_file, "of=/dev/mapper/"+uuid4])

        print("Closing dmcrypt")
        for n in range(60):
            try:
                time.sleep(1)
                subprocess.check_output(["cryptsetup", "close", uuid4])
                break
            except subprocess.CalledProcessError:
                print("Retrying...")
        else:
            raise Exception("Couldn't close dmcrypt properly")

        subprocess.check_output(["losetup", "-d", loop_device])
        s.send((0).to_bytes(1, "big"))
        print("done")
        s.close()
    except Exception as e:
        subprocess.run(["dd", "if=" + src_file, "of=/dev/mapper/"+uuid4])
        subprocess.run(["cryptsetup", "close", uuid4])
        subprocess.run(["losetup", "-d", loop_device])
        s.send((1).to_bytes(1, "big"))
        s.close()
        print(e)

    conn.close()
