op bitbucket-pull-request-list(repo_slug: String) -> Any
  description "List the open pull requests on one repository in this connection's workspace, newest activity first. Returns OPEN pull requests only — that is Bitbucket's default and this connector declares no `state` parameter to change it (see its header note), so merged and declined pull requests are not returned. Returns Bitbucket's first page only; the response's `next` field carries the URL of the following page. Each value's `id` is what bitbucket-pull-request-get, -comment and -approve take. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.bitbucket.org/2.0"
  workspace = "{workspace}"
  url = fmt("{base}/repositories/{workspace}/{repo_slug}/pullrequests")
  response = http.request(method: "GET", url)
  return response
