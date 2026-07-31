op trello-board-list -> Any
  description "List every board the token's own member can see, newest activity first. Takes no argument — `me` resolves to whoever the token belongs to. Also this connector's `verify`: a bounded read that runs unattended and needs nothing configured beyond the credential pair"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.trello.com/1"
  url = fmt("{base}/members/me/boards")
  response = http.request(method: "GET", url)
  return response
