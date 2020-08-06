# Serbo

Minecraft Server Manager in Rust

# How to use

To manage one or more servers, you must first create a Manager.
This struct will allow you to control and manage multiple servers.

A manager requires three arguments: A folder to contain the files of the servers you are managing, a version folder, which contains folders containing server files that correspond to the versions of Minecraft that you are supporting (they should be named as such: 1.16.1, 1.15.2 ...), and a jar_name, which is the name of the jarfile that serbo should execute to start the server.

You call methods on this manager to create, delete, start, stop, change a server's version, and to obtain a reference to a struct that represents an online server called an Instance.

With an Instance, you have access to methods that can access stdout (the server output), send a command to the server (via stdin), or stop that specific server. 