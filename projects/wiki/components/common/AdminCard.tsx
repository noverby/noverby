import React from 'react';
import { Avatar, Card, CardHeader, Typography } from '@mui/material';
import { SupervisorAccount } from '@mui/icons-material';

const AdminCard = ({
  children,
  title,
}: {
  children: React.ReactNode | React.ReactNode[];
  title: string;
}) => (
  <Card sx={{ m: 0, bgcolor: 'primary.main' }}>
    <CardHeader
      title={<Typography variant="h5">{title}</Typography>}
      avatar={
        <Avatar
          sx={{
            bgcolor: 'secondary.main',
          }}
        >
          <SupervisorAccount />
        </Avatar>
      }
    />
    {children}
  </Card>
);

export default AdminCard;
