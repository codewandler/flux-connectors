op bitbucket-repository-get(repo_slug: String) -> Any
  description "Read one repository in this connection's workspace by its slug, with its main branch, privacy, project and size. Use it to confirm a repository exists and to learn its default branch before opening a pull request against it. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.bitbucket.org/2.0"
  workspace = "{workspace}"
  url = fmt("{base}/repositories/{workspace}/{repo_slug}")
  response = http.request(method: "GET", url)
  return response
