#!/bin/bash
function deploy_server() {
  echo "Deploying..."
  systemctl restart nginx
}
deploy_server
