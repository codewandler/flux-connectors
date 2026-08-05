op notion-user-me -> Any
  description "Get the bot user this integration authenticates as, confirming the token resolves and naming the workspace it belongs to. Takes no parameters. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/message`, its error code at `/code` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.notion.com"
  url = fmt("{base}/v1/users/me")
  Notion_Version = "2022-06-28"
  response = http.request(headers: { "Notion-Version": Notion_Version }, method: "GET", url)
  return response
