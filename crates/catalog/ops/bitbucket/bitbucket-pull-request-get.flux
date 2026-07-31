op bitbucket-pull-request-get(repo_slug: String, pull_request_id: Number) -> Any
  description "Read one pull request by its number, including its description, its source and destination branches, its current state and who is participating. Reads a merged or declined pull request too, unlike bitbucket-pull-request-list. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.bitbucket.org/2.0"
  workspace = "{workspace}"
  url = fmt("{base}/repositories/{workspace}/{repo_slug}/pullrequests/{pull_request_id}")
  response = http.request(method: "GET", url)
  return response
