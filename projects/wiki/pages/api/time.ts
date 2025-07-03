import { NextApiRequest, NextApiResponse } from 'next';

const handler = async (req: NextApiRequest, res: NextApiResponse) =>
  res.json({
    time: new Date(),
  });

export default handler;
