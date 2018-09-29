# Updating your instance

To update your instance, run these commands with `plume` user if you created it, or with your default user, in the Plume directory.

```
git pull origin master

# If you are using sysvinit
[See section: Sysvinit integration](https://github.com/Plume-org/Plume/blob/master/docs/INSTALL.md#sysvinit-integration).
sudo service plume restart

# If you are using systemd 
[See section: Systemd integration](https://github.com/Plume-org/Plume/blob/master/docs/INSTALL.md#systemd-integration) .
sudo systemctl restart plume
```

That's it!
