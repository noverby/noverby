import React from 'react';
import { Card, CardHeader, Typography } from '@mui/material';

const HeaderCard = ({
  children,
  title,
  avatar,
  subtitle,
  action,
}: {
  children?: React.ReactNode;
  title: string;
  avatar?: React.ReactNode;
  subtitle?: string;
  action?: React.ReactNode;
}) => (
  <Card sx={{ m: 0 }}>
    <CardHeader
      action={action}
      subheader={subtitle}
      title={<Typography>{title}</Typography>}
      avatar={avatar}
    />
    {children}
  </Card>
);

export default HeaderCard;
