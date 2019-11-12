[github](https://github.com/Neuroborus/p2p_file_sharing)


Files
=======================================
File name           | File content
--------------------|----------------------
src/daemon.rs       | Daemon that executing commands from the client
src/client.rs       | Console client that sending commands to the daemon
src/lib.rs          | Contains libraries, constants and user data structures


User documentation, client commands
=======================================
Command                         | What does this command do
--------------------------------|----------------------
scan                            | Scans the network for files that can be downloaded
ls                              | Show files that can be downloaded
status                          | Show distributed files
share "file_path\file.dat"      | Share a file with a network
download "save_path" -f FileName| Download a downloadable file (path is optional)


Technical documentation
=======================================

Data types
---------------------------------------
Data type                       | What is it
--------------------------------|----------------------
enum Command                    | A command that is serialized in the client and sent to the daemon
enum Answer                     | The response to the command is serialized in the daemon and sent to the client
struct DataTemp                 | Contain 2 HashMap: of available for tranfer and download files


Daemon functions
---------------------------------------
Function                        | What does this function do
--------------------------------|----------------------
command_processor(...)          | Processing command from client
get_this_daemon_ip(...)         | Getting IP of current daemon thread
multicast_responder(...)        | Responds to multicast requests from other daemons
multicast_receiver(...)         | Receives a response to a multicast from demons


