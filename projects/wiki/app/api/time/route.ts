import { NextResponse } from 'next/server';

export const GET = async () =>
  new NextResponse(
    JSON.stringify({
      time: new Date(),
    })
  );
