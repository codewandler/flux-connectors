op bitbucket-pull-request-approve(repo_slug: String, pull_request_id: Number) -> Any
  description "Approve a pull request as the account this connection's token belongs to. This records a review that a human may not have performed, and in a repository with an approval merge check it can release a merge that gate was configured to hold — treat it as an assertion about a review, not as a bookmark. Bitbucket answers a repeat approval with 409 rather than approving twice. Sends no body: the approver is the token's own account. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message` in the response body."
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://api.bitbucket.org/2.0"
  workspace = "{workspace}"
  url = fmt("{base}/repositories/{workspace}/{repo_slug}/pullrequests/{pull_request_id}/approve")
  response = http.request(method: "POST", url)
  return response
