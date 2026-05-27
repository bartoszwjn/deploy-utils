if [ -L "$1" ]; then
    deployed=$(realpath "$1")
    if [ "$1" = /nix/var/nix/profiles/system ]; then
        inner=$(dirname "$(realpath "$deployed/activate")")
        active=$(realpath /run/current-system)
        if [ "$inner" = "$active" ]; then
            printf "valid;%s" "$deployed"
        else
            printf "needs reboot;%s" "$deployed"
        fi
    else
        printf "valid;%s" "$deployed"
    fi
elif [ -e "$1" ]; then
    printf "invalid;"
else
    printf "missing;"
fi
