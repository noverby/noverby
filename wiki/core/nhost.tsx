import { NhostClient } from "@nhost/react";

const nhost = new NhostClient({
	subdomain: process.env.PUBLIC_NHOST_SUBDOMAIN,
	region: process.env.PUBLIC_NHOST_REGION,
});

export { nhost };
