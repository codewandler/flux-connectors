op linear-viewer -> Any
  description "Read the Linear user this connection's API key belongs to: their id, name, email and admin flag. Takes no arguments. Use it to confirm the key works and to learn whose access this connection actually has, since a Linear personal key cannot be narrowed below its owner's permissions. Also this connector's verify operation. Linear answers every failure with HTTP 200 and an `errors` array beside a null `data`, so check `errors` before reading `data`"
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://api.linear.app"
  url = fmt("{base}/graphql")
  content_type = "application/json"
  query = """query Viewer {
  viewer {
    id
    name
    displayName
    email
    admin
  }
}
"""
  payload = { query }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
