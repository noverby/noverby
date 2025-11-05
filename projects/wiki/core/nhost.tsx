import { NhostClient } from "@nhost/nextjs";

const nhost = new NhostClient({
	subdomain: process.env.NHOST_SUBDOMAIN,
	region: process.env.NHOST_REGION,
});

export { nhost };
