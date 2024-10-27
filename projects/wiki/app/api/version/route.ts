import { NextResponse } from 'next/server';

export const GET = async () =>
  new NextResponse(
    JSON.stringify({
      commit: process.env.VERCEL_GIT_COMMIT_SHA,
      env: process.env.NODE_ENV,
    })
  );
