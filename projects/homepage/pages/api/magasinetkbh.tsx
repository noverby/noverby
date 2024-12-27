import type { NextApiRequest, NextApiResponse } from 'next'

type ErrorResponse = {
  message: string
}

export default async function handler(
  req: NextApiRequest,
  res: NextApiResponse<string | ErrorResponse>
) {
  if (req.method !== 'GET') {
    return res.status(405).json({ message: 'Method not allowed' });
  }

  try {
    const response = await fetch(
      'http://kbh-rss-feed.s3-website-us-east-1.amazonaws.com/byens-rum-liv--mode-range-limit.xml'
    );

    if (!response.ok) {
      throw new Error(`HTTP error! status: ${response.status}`);
    }

    const data = await response.text();

    // Set appropriate headers
    res.setHeader('Content-Type', 'application/xml');
    res.setHeader('Cache-Control', 's-maxage=60, stale-while-revalidate');

    return res.status(200).send(data);
  } catch (error) {
    console.error('Error fetching RSS feed:', error instanceof Error ? error.message : error);
    return res.status(500).json({ message: 'Error fetching RSS feed' });
  }
}