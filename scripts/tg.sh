#!/bin/bash
# Telegram reporting script — session updates for danku13
BOT_TOKEN="8655788498:AAFYPzWx19rI-CIsgJ54NpXaPUwOljCOq9E"
CHAT_ID="274002630"
SESSION_URL="https://chat.z.ai/s/web-0a539c42-84fd-4916-9a97-970c8052c51d"
TEXT="$1"
curl -s -X POST "https://api.telegram.org/bot${BOT_TOKEN}/sendMessage" \
  -d chat_id="${CHAT_ID}" \
  -d parse_mode="HTML" \
  --data-urlencode "text=${SESSION_URL}

${TEXT}" | head -c 400
