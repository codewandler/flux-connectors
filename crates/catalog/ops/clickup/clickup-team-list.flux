op clickup-team-list -> Any
  description "List the workspaces (called \"teams\" in ClickUp's API) the token can see. Takes no parameters; used as the verify read to prove a token resolves, and as the source of the workspace-level context a space id is nested under"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.clickup.com/api/v2"
  url = fmt("{base}/team")
  response = http.request(method: "GET", url)
  return response
