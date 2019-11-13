[github](https://github.com/Neuroborus/p2p_file_sharing)
`cargo doc --open` for automatic documentation generation

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
enum FileSizeorInfo             | Stores an action downloadable peer want to do
enum EnumAnswer                 | If downloadable peer asked file size we answering with size or file not exist
struct DataTemp                 | Contain 2 HashMap: of available for transfer and download files
struct TransferGuard            | Adding peer to transferring vector when created, and removing peer from vector while destroying
struct FirstRequest             | Serialized request from daemon which want to get file size or start download a file
struct FileInfo                 | Store which blocks downloadable
struct AnswerToFirstRequest     | Stores filename and answer to size request


Daemon functions
---------------------------------------
Function                        | What does this function do
--------------------------------|----------------------
command_processor(...)          | Processing command from client
get_this_daemon_ip(...)         | Getting IP of current daemon thread
multicast_responder(...)        | Responds to multicast requests from other daemons
multicast_receiver(...)         | Receives a response to a multicast from daemons
share_responder(...)            | Processing queries from other daemons
handle_first_share_request(...) | Process first query which is get file size or start download a file
share_to_peer(...)              | Start sharing the file to other daemon
get_fsize_on_each_peer(...)     | Fill the HashMap of peer and his file size
remove_other_fsizes_in_vec(...) | Leave the most used file size and peer
fill_block_watcher(...)         | Split up the file in blocks and peers
download_request(...)           | Send the download file request to other daemons
download_from_peer(...)         | Download a specific file blocks from peer
