op confluence-space-get(id: String) -> Any
  description "Get one space by its id, with its key, name, status and home page id. Addressed by the numeric space `id` from `confluence-space-list`, not by the space key that appears in a Confluence URL"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{site}.atlassian.net/wiki"
  url = fmt("{base}/api/v2/spaces/{id}")
  response = http.request(method: "GET", url)
  return response
