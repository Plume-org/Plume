# Updating your instance

To update your instance, run these commands with `plume` user if you created it, or with your default user, in the Plume directory.

```bash
git pull origin master
cargo install --force && cargo install --path plume-cli --force

# Run the migrations
diesel migration run

# If you are using sysvinit
sudo service plume restart

# If you are using systemd
sudo systemctl restart plume
```

That's it!
