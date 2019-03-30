#!/bin/bash
CIRCLE_TOKEN=d5d379e1e88e6d8751377e15b92d5a357bc70da5

curl --user ${CIRCLE_TOKEN}: \
    --request POST \
    --form revision=c4cc4ad3b2f95f2aa1f7bebfbe03858faa1e850c\
    --form config=@config.yml \
    --form notify=false \
        https://circleci.com/api/v1.1/project/github/fdb-hiroshima/Plume/tree/circleci \
	2>/dev/null | grep message
