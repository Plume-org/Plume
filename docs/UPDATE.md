# Updating your instance

To update your instance, run these commands with `plume` user if you created it, or with your default user, in the Plume directory.

```
git pull origin master

# If you are using sysvinit
sudo service plume restart

# If you are using systemd 
sudo systemctl restart plume
```

That's it!
