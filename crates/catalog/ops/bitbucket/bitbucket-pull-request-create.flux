op bitbucket-pull-request-create(repo_slug: String, title: String, source_branch: String, destination_branch: String) -> Any
  description "Open a pull request from one branch to another in a repository of this connection's workspace. Both branches must already exist and be pushed. Bitbucket does not deduplicate: opening the same source-to-destination pull request twice creates two of them, and notifies the repository's watchers twice. The created pull request, with its assigned `id`, is in the response. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message` in the response body."
  risk "medium"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://api.bitbucket.org/2.0"
  workspace = "{workspace}"
  url = fmt("{base}/repositories/{workspace}/{repo_slug}/pullrequests")
  content_type = "application/json"
  payload = { destination: { branch: { name: destination_branch } }, source: { branch: { name: source_branch } }, title }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
