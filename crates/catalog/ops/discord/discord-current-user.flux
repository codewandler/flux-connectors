op discord-current-user -> Any
  description "Read the bot user this token authenticates as — its snowflake id, username and application flags. Takes no argument. Also this connector's `verify`: the cheapest call that proves the token, the `Bot ` scheme word and the base URL are all correct together, and the one to run first when any other operation returns 401"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://discord.com/api/v10"
  url = fmt("{base}/users/@me")
  response = http.request(method: "GET", url)
  return response
