# Updating your instance

To update your instance, run these commands with `plume` user if you created it, or with your default user, in the Plume directory.

```
git pull origin master

# If you are not using systemd
cargo run

# If you are using systemd
service plume restart
```

That's it!
