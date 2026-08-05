op bitbucket-repository-list -> Any
  description "List the repositories in this connection's workspace, with each one's slug, name, description, main branch and privacy. Takes no argument at all: the workspace is pinned when the connection is made, so this reads exactly the workspace the operator chose and no other. Returns Bitbucket's first page only; this connector declares no page or filter parameters, and the response's `next` field carries the URL of the following page. The `slug` returned here is what every other operation in this connector takes as `repo_slug`. Also this connector's `verify`: a bounded read that runs unattended and needs nothing beyond the token and the pinned workspace. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.bitbucket.org/2.0"
  workspace = "{workspace}"
  url = fmt("{base}/repositories/{workspace}")
  response = http.request(method: "GET", url)
  return response
